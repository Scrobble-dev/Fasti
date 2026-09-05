import { mount } from "svelte";
import App from "./App.svelte";
import "./global.css";

const target = document.getElementById("app");
if (!target) throw new Error("Fasti application root is missing");

mount(App, { target });
