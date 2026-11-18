import { useEffect, useMemo, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import {
  EVENT_PTT,
  EVENT_STATE,
  INITIAL_STATE,
  MAX_MHZ,
  MIN_MHZ,
  STATIONS,
  stationAt,
  type RadioState,
} from "./shared";

// Escala do dial: uma marca a cada 0,1 MHz, rótulo a cada 1 MHz.
const TICKS = Array.from({ length: Math.round((MAX_MHZ - MIN_MHZ) * 10) + 1 }, (_, i) => {
  const mhz = MIN_MHZ + i / 10;
  return { mhz, major: Math.abs(mhz - Math.round(mhz)) < 0.01 };
});

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const fmt = (mhz: number) => mhz.toFixed(1);

export function RadioView() {
  const [radio, setRadio] = useState<RadioState>(INITIAL_STATE);
  const [clock, setClock] = useState(() => new Date());
  const dragRef = useRef<{ kind: "tune" | "volume"; x: number; start: number } | null>(null);
  const pttHeld = useRef(false);

  // Relógio do display.
  useEffect(() => {
    const id = setInterval(() => setClock(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  // Estado sempre publicado para a ilha.
  useEffect(() => {
    void emit(EVENT_STATE, radio);
  }, [radio]);

  // PTT vindo da ilha (botão segurado lá).
  useEffect(() => {
    const unlisten = listen<{ down: boolean }>(EVENT_PTT, (e) => {
      setRadio((r) => ({ ...r, transmitting: e.payload.down && r.power }));
    });
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  // Atalho de teclado: segurar Espaço = PTT.
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.code === "Space" && !e.repeat && !pttHeld.current) {
        pttHeld.current = true;
        setRadio((r) => ({ ...r, transmitting: r.power }));
      }
    };
    const up = (e: KeyboardEvent) => {
      if (e.code === "Space") {
        pttHeld.current = false;
        setRadio((r) => ({ ...r, transmitting: false }));
      }
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, []);

  const station = radio.power ? stationAt(radio.mhz) : null;
  const channelLabel = !radio.power
    ? ""
    : station?.name ?? "Sintonia livre";

  const setMhz = (mhz: number) => {
    const snapped = Math.round(clamp(mhz, MIN_MHZ, MAX_MHZ) * 10) / 10;
    setRadio((r) => ({ ...r, mhz: snapped, channel: stationAt(snapped)?.name ?? "Sintonia livre" }));
  };

  const tune = (dir: 1 | -1) => setMhz(radio.mhz + dir * 0.1);

  const jumpStation = (dir: 1 | -1) => {
    const idx = STATIONS.findIndex((s) => Math.abs(s.mhz - radio.mhz) < 0.05);
    const next = STATIONS[(idx + dir + STATIONS.length) % STATIONS.length];
    setMhz(next.mhz);
  };

  const togglePower = () => setRadio((r) => ({ ...r, power: !r.power, transmitting: false }));

  const onPointerDown = (kind: "tune" | "volume") => (e: RPointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    dragRef.current = {
      kind,
      x: e.clientX,
      start: kind === "tune" ? radio.mhz : radio.volume,
    };
  };

  const onPointerMove = (e: RPointerEvent<HTMLDivElement>) => {
    const d = dragRef.current;
    if (!d) return;
    const dx = e.clientX - d.x;
    if (d.kind === "tune") {
      setMhz(d.start + dx * 0.02); // 50 px ≈ 1 MHz
    } else {
      setRadio((r) => ({ ...r, volume: clamp(Math.round(d.start + dx * 0.5), 0, 100) }));
    }
  };

  const onPointerUp = () => {
    dragRef.current = null;
  };

  const onWheel = (e: React.WheelEvent) => {
    tune(e.deltaY < 0 ? 1 : -1);
  };

  // Posição do ponteiro no dial (0..1).
  const dialPos = useMemo(() => (radio.mhz - MIN_MHZ) / (MAX_MHZ - MIN_MHZ), [radio.mhz]);

  const time = clock.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });

  return (
    <div className="radio-window">
      <div className="radio-body" data-power={radio.power}>
        {/* Grade do alto-falante */}
        <div className="grille" aria-hidden>
          <div className="grille-dots" />
        </div>

        <div className="panel">
          <header className="brand-row">
            <span className="brand">GodMesh</span>
            <span className="brand-sub">Rádio de mesa · protótipo</span>
          </header>

          {/* Display */}
          <section className="display" aria-live="polite">
            <div className="display-top">
              <span className="led" data-on={radio.power && radio.transmitting} />
              <span className="display-label">{radio.transmitting ? "TX" : "RX"}</span>
              <span className="display-clock">{radio.power ? time : "--:--"}</span>
            </div>
            <div className="freq">
              <span className="freq-value">{radio.power ? fmt(radio.mhz) : "--.-"}</span>
              <span className="freq-unit">MHz</span>
            </div>
            <div className="channel">{channelLabel}</div>
            <div className="signal" data-active={radio.power && station !== null}>
              {[0, 1, 2, 3, 4].map((i) => (
                <i key={i} style={{ height: `${30 + i * 14}%` }} />
              ))}
              <span className="signal-tag">{station?.tag ?? ""}</span>
            </div>
          </section>

          {/* Dial */}
          <section
            className="dial"
            onPointerDown={onPointerDown("tune")}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onWheel={onWheel}
            role="slider"
            aria-label="Sintonia"
            aria-valuemin={MIN_MHZ}
            aria-valuemax={MAX_MHZ}
            aria-valuenow={radio.mhz}
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "ArrowRight") tune(1);
              if (e.key === "ArrowLeft") tune(-1);
            }}
          >
            <div className="dial-scale" style={{ transform: `translateX(${50 - dialPos * 100}%)` }}>
              {TICKS.map((t) => (
                <span
                  key={t.mhz.toFixed(1)}
                  className={t.major ? "tick major" : "tick"}
                  style={{ left: `${((t.mhz - MIN_MHZ) / (MAX_MHZ - MIN_MHZ)) * 100}%` }}
                >
                  {t.major && <b>{Math.round(t.mhz)}</b>}
                </span>
              ))}
              {STATIONS.map((s) => (
                <span
                  key={s.mhz}
                  className="station-mark"
                  style={{ left: `${((s.mhz - MIN_MHZ) / (MAX_MHZ - MIN_MHZ)) * 100}%` }}
                  title={s.name}
                />
              ))}
            </div>
            <div className="needle" />
          </section>

          {/* Controles */}
          <section className="controls">
            <button className="btn round" onClick={togglePower} aria-pressed={radio.power}>
              <span className="power-icon" />
              <small>Liga</small>
            </button>

            <button className="btn" onClick={() => jumpStation(-1)} aria-label="Canal anterior">
              ‹‹
            </button>
            <button className="btn" onClick={() => jumpStation(1)} aria-label="Próximo canal">
              ››
            </button>

            <div
              className="knob volume"
              onPointerDown={onPointerDown("volume")}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              role="slider"
              aria-label="Volume"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={radio.volume}
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "ArrowUp")
                  setRadio((r) => ({ ...r, volume: clamp(r.volume + 5, 0, 100) }));
                if (e.key === "ArrowDown")
                  setRadio((r) => ({ ...r, volume: clamp(r.volume - 5, 0, 100) }));
              }}
              title={`Volume ${radio.volume}`}
            >
              <div className="knob-cap" style={{ transform: `rotate(${-135 + (radio.volume / 100) * 270}deg)` }}>
                <i />
              </div>
              <small>Volume {radio.volume}</small>
            </div>

            <button
              className="ptt"
              disabled={!radio.power}
              onPointerDown={(e) => {
                e.currentTarget.setPointerCapture(e.pointerId);
                setRadio((r) => ({ ...r, transmitting: r.power }));
              }}
              onPointerUp={() => setRadio((r) => ({ ...r, transmitting: false }))}
              onPointerCancel={() => setRadio((r) => ({ ...r, transmitting: false }))}
              data-on={radio.transmitting}
            >
              {radio.transmitting ? "Transmitindo" : "Segure · PTT"}
            </button>
          </section>

          {/* Predefinições */}
          <section className="presets">
            {STATIONS.map((s, i) => (
              <button
                key={s.mhz}
                className="preset"
                data-active={radio.power && Math.abs(s.mhz - radio.mhz) < 0.05}
                onClick={() => {
                  setRadio((r) => ({ ...r, power: true }));
                  setMhz(s.mhz);
                }}
              >
                <b>{i + 1}</b>
                <span>{fmt(s.mhz)}</span>
              </button>
            ))}
          </section>
        </div>
      </div>
    </div>
  );
}
