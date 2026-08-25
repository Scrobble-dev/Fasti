import { mount } from "svelte";
import App from "./App.svelte";
import "./global.css";
import { applyTheme, resolveTheme } from "./theme.js";

applyTheme(resolveTheme(), false);

const target = document.getElementById("app");
if (!target) throw new Error("Fasti application root is missing");

mount(App, { target });
