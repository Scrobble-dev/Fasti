import { cssVariables } from "@fasti/tokens";
import { mount } from "svelte";
import App from "./App.svelte";
import "./global.css";

const tokens = document.createElement("style");
tokens.textContent = cssVariables;
document.head.append(tokens);

const target = document.getElementById("app");
if (!target) throw new Error("Fasti application root is missing");

mount(App, { target });
