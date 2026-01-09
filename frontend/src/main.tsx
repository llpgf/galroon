
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import "./index.css";
import "./styles/globals.css"; // Must be AFTER index.css to override Tailwind defaults

createRoot(document.getElementById("root")!).render(<App />);
