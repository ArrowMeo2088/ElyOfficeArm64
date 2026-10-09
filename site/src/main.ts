import "./style.css";

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("index.html has no #app");
app.textContent = "Ely Office";
