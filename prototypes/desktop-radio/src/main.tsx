import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import { RadioView } from "./RadioView";
import { IslandView } from "./IslandView";

// Mesma base de bundle para as duas janelas; a URL decide qual renderizar.
const view = new URLSearchParams(window.location.search).get("view");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {view === "island" ? <IslandView /> : <RadioView />}
  </React.StrictMode>,
);
