import { useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { EVENT_PTT, EVENT_STATE, INITIAL_STATE, type RadioState } from "./shared";

// Emulação de Dynamic Island no macOS. Não há API pública para a Dynamic Island:
// esta janela borderless, sempre no topo, fica sobre o recorte (notch) da tela
// e alterna entre a forma compacta e a expandida.
const COLLAPSED = { w: 240, h: 38 };
const EXPANDED = { w: 340, h: 118 };

/** Posiciona a janela centralizada no topo da tela principal, com o tamanho pedido. */
async function placeAtTopCenter(w: number, h: number) {
  const win = getCurrentWindow();
  const monitor = await currentMonitor();
  const scale = monitor?.scaleFactor ?? 2;
  const screenW = monitor ? monitor.size.width / scale : 1440;
  await win.setSize(new LogicalSize(w, h));
  await win.setPosition(new LogicalPosition(Math.round((screenW - w) / 2), 0));
}

export function IslandView() {
  const [radio, setRadio] = useState<RadioState>(INITIAL_STATE);
  const [expanded, setExpanded] = useState(false);
  const pttDown = useRef(false);

  // Fundo transparente só nesta janela.
  useEffect(() => {
    document.documentElement.classList.add("island");
    return () => document.documentElement.classList.remove("island");
  }, []);

  // Posição inicial.
  useEffect(() => {
    void placeAtTopCenter(COLLAPSED.w, COLLAPSED.h);
  }, []);

  // Redimensiona ao expandir/recolher.
  useEffect(() => {
    const s = expanded ? EXPANDED : COLLAPSED;
    void placeAtTopCenter(s.w, s.h);
  }, [expanded]);

  // Recebe o estado do rádio.
  useEffect(() => {
    const unlisten = listen<RadioState>(EVENT_STATE, (e) => setRadio(e.payload));
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  const startPtt = () => {
    if (!radio.power || pttDown.current) return;
    pttDown.current = true;
    void emit(EVENT_PTT, { down: true });
  };
  const stopPtt = () => {
    if (!pttDown.current) return;
    pttDown.current = false;
    void emit(EVENT_PTT, { down: false });
  };

  const active = radio.power && radio.transmitting;
  const freq = radio.power ? radio.mhz.toFixed(1) : "--.-";
  const title = !radio.power ? "Desligado" : radio.transmitting ? "Transmitindo" : radio.channel;

  return (
    <div className="island-root" data-expanded={expanded} data-active={active}>
      <div className="island-pill" onClick={() => setExpanded((v) => !v)} role="button" tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && setExpanded((v) => !v)}>
        {!expanded ? (
          <div className="island-compact">
            <span className="island-freq">{freq}</span>
            <span className="island-notch-gap" />
            <Wave on={radio.power} live={active || radio.power} />
          </div>
        ) : (
          <div className="island-expanded">
            <div className="island-head">
              <Wave on={radio.power} live={active || radio.power} />
              <div className="island-text">
                <span className="island-title">{title}</span>
                <span className="island-sub">{freq} MHz · Vol {radio.volume}</span>
              </div>
            </div>
            <button
              className="island-ptt"
              data-on={active}
              disabled={!radio.power}
              onClick={(e) => e.stopPropagation()}
              onPointerDown={(e) => {
                e.stopPropagation();
                e.currentTarget.setPointerCapture(e.pointerId);
                startPtt();
              }}
              onPointerUp={stopPtt}
              onPointerCancel={stopPtt}
            >
              {active ? "Transmitindo…" : "Segure para falar"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function Wave({ on, live }: { on: boolean; live: boolean }) {
  return (
    <span className="island-wave" data-on={on} data-live={live} aria-hidden>
      {[0, 1, 2, 3].map((i) => (
        <i key={i} style={{ animationDelay: `${i * 0.12}s` }} />
      ))}
    </span>
  );
}
