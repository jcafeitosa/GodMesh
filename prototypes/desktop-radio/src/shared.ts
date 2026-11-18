// Estado e eventos compartilhados entre a janela do rádio e a ilha.
// Tudo é simulado: não há áudio nem rede neste protótipo.

export interface Station {
  mhz: number;
  name: string;
  tag: string;
}

export const STATIONS: Station[] = [
  { mhz: 88.1, name: "Canal 01 · Operações", tag: "GMP" },
  { mhz: 92.4, name: "Canal 02 · Logística", tag: "GMP" },
  { mhz: 98.5, name: "Canal 03 · Plantão", tag: "GMP" },
  { mhz: 101.9, name: "Canal 04 · Campo", tag: "GMP" },
  { mhz: 106.3, name: "Canal 05 · Voz geral", tag: "GMP" },
  { mhz: 107.7, name: "Canal 06 · Pesquisa", tag: "GMP" },
];

export const MIN_MHZ = 87.5;
export const MAX_MHZ = 108.0;

/** Estação mais próxima, se estiver a até 0,2 MHz; senão null (sintonia livre). */
export function stationAt(mhz: number): Station | null {
  let best: Station | null = null;
  let bestDist = Infinity;
  for (const s of STATIONS) {
    const d = Math.abs(s.mhz - mhz);
    if (d < bestDist) {
      best = s;
      bestDist = d;
    }
  }
  return best && bestDist <= 0.2 ? best : null;
}

export interface RadioState {
  power: boolean;
  mhz: number;
  volume: number; // 0..100
  transmitting: boolean;
  channel: string; // nome do canal ou "Sintonia livre"
}

/** main → island */
export const EVENT_STATE = "radio-state";
/** island → main */
export const EVENT_PTT = "radio-ptt";

export const INITIAL_STATE: RadioState = {
  power: true,
  mhz: 98.5,
  volume: 55,
  transmitting: false,
  channel: STATIONS[2].name,
};
