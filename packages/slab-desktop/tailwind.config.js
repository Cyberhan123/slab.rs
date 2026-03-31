/** @type {import('tailwindcss').Config} */
export default {
  // Tailwind v4 theme tokens live in `src/styles/globals.css` via `@theme`.
  // Keep this file as a minimal compatibility stub so we do not maintain a
  // second, divergent theme source in JavaScript.
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
    // Include slab-components source so Tailwind scans workspace package classes
    "../slab-components/src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {},
  plugins: [],
}
