//! ADR-057 — cursor opaco versionado vinculado a escopo, principal e filtros.
//!
//! Cursor sem partição ou de epoch incomparável é **rejeitado**; não existe caminho
//! de compatibilidade que preencha a partição ausente nem que traduza epoch.

/// Sentinela de "sem partição" no contrato de escopo.
const NO_PARTITION: u32 = u32::MAX;

/// Escopo durável ao qual um cursor pertence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorScope {
    tenant: String,
    stream: String,
    partition: u32,
    authority_epoch: u64,
}

impl CursorScope {
    /// # Errors
    /// `CursorError::PartitionRequired` quando a partição é a sentinela de ausência.
    pub fn new(tenant: &str, stream: &str, partition: u32, authority_epoch: u64) -> Result<Self, CursorError> {
        if partition == NO_PARTITION {
            return Err(CursorError::PartitionRequired);
        }
        Ok(Self { tenant: tenant.to_owned(), stream: stream.to_owned(), partition, authority_epoch })
    }

    #[must_use]
    pub const fn partition(&self) -> u32 {
        self.partition
    }

    #[must_use]
    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }
}

/// Cursor de leitura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    scope: CursorScope,
    principal: String,
    filters: String,
    profile: String,
    retention: String,
    version: u16,
}

impl Cursor {
    /// # Errors
    /// Propaga erro de escopo e recusa campos obrigatórios vazios.
    pub fn new(
        scope: CursorScope,
        principal: &str,
        filters: &str,
        profile: &str,
        retention: &str,
    ) -> Result<Self, CursorError> {
        if principal.is_empty() || profile.is_empty() || retention.is_empty() {
            return Err(CursorError::EmptyField);
        }
        Ok(Self {
            scope,
            principal: principal.to_owned(),
            filters: filters.to_owned(),
            profile: profile.to_owned(),
            retention: retention.to_owned(),
            version: 1,
        })
    }

    #[must_use]
    pub const fn partition(&self) -> u32 {
        self.scope.partition()
    }

    #[must_use]
    pub const fn authority_epoch(&self) -> u64 {
        self.scope.authority_epoch()
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub fn is_comparable_with(&self, other: &CursorScope) -> bool {
        self.scope.authority_epoch == other.authority_epoch
            && self.scope.partition == other.partition
            && self.scope.tenant == other.tenant
            && self.scope.stream == other.stream
    }

    /// # Errors
    /// `EpochIncomparable`, `PartitionMismatch` ou `TenantMismatch` conforme a divergência.
    pub fn check_against(&self, current: &CursorScope) -> Result<(), CursorError> {
        if self.scope.tenant != current.tenant || self.scope.stream != current.stream {
            return Err(CursorError::TenantMismatch);
        }
        if self.scope.partition != current.partition {
            return Err(CursorError::PartitionMismatch);
        }
        if self.scope.authority_epoch != current.authority_epoch {
            return Err(CursorError::EpochIncomparable);
        }
        Ok(())
    }

    /// # Errors
    /// `PrincipalMismatch` ou `FilterMismatch` quando o cursor é reapresentado fora do vínculo.
    pub fn check_binding(&self, principal: &str, filters: &str) -> Result<(), CursorError> {
        if self.principal != principal {
            return Err(CursorError::PrincipalMismatch);
        }
        if self.filters != filters {
            return Err(CursorError::FilterMismatch);
        }
        Ok(())
    }
}

/// Erros de contrato de cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorError {
    PartitionRequired,
    EpochIncomparable,
    PartitionMismatch,
    TenantMismatch,
    PrincipalMismatch,
    FilterMismatch,
    EmptyField,
}

// --- token de cursor (wire/API) ----------------------------------------------

/// Versão do token desta fatia. Cursor de versão desconhecida é recusado; não
/// existe caminho de compatibilidade que adivinhe a versão.
pub const CURSOR_TOKEN_VERSION: u16 = 1;

/// Cursor decodificado: escopo + vínculo + posição de retomada.
///
/// O token não é assinado: não há chave nem emissão de credencial decididas. O
/// servidor reconstrói o cursor do token, mas **toda** leitura continua sendo
/// filtrada pelo `tenant_id` e pelo `principal` do grant verificado, então um
/// token adulterado não amplia escopo — no máximo aponta para outra posição
/// dentro do mesmo escopo autorizado, e a vinculação abaixo recusa o resto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedCursor {
    cursor: Cursor,
    last_seq: u64,
}

impl DecodedCursor {
    #[must_use]
    pub const fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Maior sequência já entregue nesta consulta; a próxima página começa em
    /// `last_seq + 1`. Não é checkpoint de Inbox: é posição de leitura.
    #[must_use]
    pub const fn last_seq(&self) -> u64 {
        self.last_seq
    }
}

impl Cursor {
    /// Serializa o cursor em token opaco, versionado e estável.
    ///
    /// O token carrega `(tenant, stream, partição, epoch, principal, filtros,
    /// perfil, retenção, última sequência entregue)`. Cliente não constrói nem
    /// interpreta: é string opaca. Codificação é base64url de campos prefixados
    /// por tamanho, então qualquer campo pode conter `|`, `:` ou `%` sem
    /// ambiguidade. Não há criptografia nem assinatura nesta fatia.
    #[must_use]
    pub fn encode(&self, last_seq: u64) -> String {
        // Todo campo, inclusive numérico, é `<len>:<bytes>|`: formato uniforme,
        // sem ambiguidade quando o conteúdo traz `|`, `:` ou `%`.
        let fields = [
            self.version.to_string(),
            self.scope.tenant.clone(),
            self.scope.stream.clone(),
            self.principal.clone(),
            self.filters.clone(),
            self.profile.clone(),
            self.retention.clone(),
            self.scope.partition.to_string(),
            self.scope.authority_epoch.to_string(),
            last_seq.to_string(),
        ];
        let mut payload = String::new();
        for field in &fields {
            payload.push_str(&field.len().to_string());
            payload.push(':');
            payload.push_str(field);
            payload.push('|');
        }

        format!("v{}.{}", self.version, base64url_encode(payload.as_bytes()))
    }

    /// Lê um token de cursor emitido por [`Cursor::encode`].
    ///
    /// # Errors
    /// `CursorTokenError::UnsupportedVersion` para versão diferente de
    /// [`CURSOR_TOKEN_VERSION`]; `Malformed` para token truncado, base64 ou campo
    /// numérico inválido; `PartitionRequired` para token sem partição (nunca
    /// completada por inferência); `EmptyField` para campo obrigatório vazio.
    pub fn decode(token: &str) -> Result<DecodedCursor, CursorTokenError> {
        let Some(rest) = token.strip_prefix('v') else {
            return Err(CursorTokenError::Malformed);
        };
        let Some((version, encoded)) = rest.split_once('.') else {
            return Err(CursorTokenError::Malformed);
        };
        let version: u16 = version.parse().map_err(|_| CursorTokenError::Malformed)?;
        if version != CURSOR_TOKEN_VERSION {
            return Err(CursorTokenError::UnsupportedVersion(version));
        }
        let decoded = base64url_decode(encoded).ok_or(CursorTokenError::Malformed)?;
        let payload = String::from_utf8(decoded).map_err(|_| CursorTokenError::Malformed)?;

        let mut fields = FieldReader::new(&payload);
        let version: u16 = fields
            .number()
            .ok_or(CursorTokenError::Malformed)?
            .try_into()
            .map_err(|_| CursorTokenError::Malformed)?;
        if version != CURSOR_TOKEN_VERSION {
            return Err(CursorTokenError::UnsupportedVersion(version));
        }
        let mut read = || fields.field().ok_or(CursorTokenError::Malformed);
        let tenant = read()?;
        let stream = read()?;
        let principal = read()?;
        let filters = read()?;
        let profile = read()?;
        let retention = read()?;
        let partition = fields.partition()?;
        let authority_epoch = fields.number().ok_or(CursorTokenError::Malformed)?;
        let last_seq = fields.number().ok_or(CursorTokenError::Malformed)?;
        if !fields.is_done() {
            return Err(CursorTokenError::Malformed);
        }

        let scope = CursorScope::new(tenant, stream, partition, authority_epoch)
            .map_err(CursorTokenError::from_cursor)?;
        let cursor = Cursor::new(scope, principal, filters, profile, retention)
            .map_err(CursorTokenError::from_cursor)?;
        Ok(DecodedCursor { cursor, last_seq })
    }
}

/// Leitor dos campos prefixados por tamanho (`<len>:<bytes>|`).
struct FieldReader<'a> {
    rest: &'a str,
}

impl<'a> FieldReader<'a> {
    const fn new(payload: &'a str) -> Self {
        Self { rest: payload }
    }

    fn field(&mut self) -> Option<&'a str> {
        let (len, tail) = self.rest.split_once(':')?;
        let len: usize = len.parse().ok()?;
        let body = tail.get(..len)?;
        let after = tail.get(len..)?.strip_prefix('|')?;
        self.rest = after;
        Some(body)
    }

    fn number(&mut self) -> Option<u64> {
        self.field()?.parse().ok()
    }

    fn partition(&mut self) -> Result<u32, CursorTokenError> {
        let raw = self.number().ok_or(CursorTokenError::Malformed)?;
        let partition = u32::try_from(raw).map_err(|_| CursorTokenError::Malformed)?;
        if partition == NO_PARTITION {
            return Err(CursorTokenError::PartitionRequired);
        }
        Ok(partition)
    }

    const fn is_done(&self) -> bool {
        self.rest.is_empty()
    }
}

const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url sem padding (RFC 4648 §5). Implementado aqui para não acrescentar
/// dependência só por causa de 20 linhas de codificação.
fn base64url_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).map_or(0, |b| u32::from(*b));
        let b2 = chunk.get(2).map_or(0, |b| u32::from(*b));
        let bits = (b0 << 16) | (b1 << 8) | b2;
        out.push(char::from(BASE64URL[(bits >> 18) as usize & 0x3f]));
        out.push(char::from(BASE64URL[(bits >> 12) as usize & 0x3f]));
        if chunk.len() > 1 {
            out.push(char::from(BASE64URL[(bits >> 6) as usize & 0x3f]));
        }
        if chunk.len() > 2 {
            out.push(char::from(BASE64URL[bits as usize & 0x3f]));
        }
    }
    out
}

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for byte in input.bytes() {
        let value = BASE64URL.iter().position(|c| *c == byte)?;
        acc = (acc << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).ok()?);
        }
    }
    if bits >= 6 {
        return None;
    }
    Some(out)
}

/// Erros do token de cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorTokenError {
    Malformed,
    UnsupportedVersion(u16),
    PartitionRequired,
    EmptyField,
}

impl CursorTokenError {
    const fn from_cursor(error: CursorError) -> Self {
        match error {
            CursorError::PartitionRequired => Self::PartitionRequired,
            CursorError::EmptyField => Self::EmptyField,
            // `CursorScope::new` e `Cursor::new` só falham por partição ausente ou
            // campo vazio; os demais erros pertencem à comparação, não à construção.
            CursorError::EpochIncomparable
            | CursorError::PartitionMismatch
            | CursorError::TenantMismatch
            | CursorError::PrincipalMismatch
            | CursorError::FilterMismatch => Self::Malformed,
        }
    }
}
