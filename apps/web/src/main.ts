import { cssVariables } from "@fasti/tokens";
import { mount } from "svelte";
import App from "./App.svelte";
import "./global.css";

// Global Error & Promise Rejection Diagnostics for DX
window.addEventListener("error", (event) => {
  console.group(
    "%c[Fasti UI Error]",
    "color: #d63939; font-weight: bold; font-size: 1.1em;",
  );
  console.error("Message:", event.message);
  console.error(
    "Location:",
    `${event.filename}:${event.lineno}:${event.colno}`,
  );
  if (event.error) {
    console.error("Error Details:", event.error);
    if (event.error.stack) console.error("Stack:\n" + event.error.stack);
  }
  console.groupEnd();
});

window.addEventListener("unhandledrejection", (event) => {
  console.group(
    "%c[Fasti UI Unhandled Rejection]",
    "color: #f76707; font-weight: bold; font-size: 1.1em;",
  );
  console.error("Reason:", event.reason);
  if (event.reason instanceof Error && event.reason.stack) {
    console.error("Stack:\n" + event.reason.stack);
  }
  console.groupEnd();
});

const tokens = document.createElement("style");
tokens.textContent = cssVariables;
document.head.append(tokens);

const target = document.getElementById("app");
if (!target) throw new Error("Fasti application root is missing");

mount(App, { target });
