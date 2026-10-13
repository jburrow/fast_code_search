/** @type {import('tailwindcss').Config} */
module.exports = {
    darkMode: "class",
    content: [
        "./static/**/*.{html,js}",
    ],
    theme: {
        extend: {
            // Every colour is a design token from static/app.css, so themes
            // restyle utility classes too. Names are the ones the pages use.
            colors: {
                "black": "var(--ink)",
                "white": "var(--surface)",
                "on-ink": "var(--on-ink)",
                "background": "var(--page)",
                "on-background": "var(--text)",
                "on-surface": "var(--text)",
                "surface-container-lowest": "var(--surface)",
                "surface-container-low": "var(--surface-1)",
                "surface-container-high": "var(--surface-3)",
                "surface-container-highest": "var(--surface-4)",
                "outline": "var(--text-4)",
                "outline-variant": "var(--border)",
                "primary": "var(--accent-ink)",
                "primary-container": "var(--accent)",
            },
            fontFamily: {
                "headline": ["Space Grotesk", "sans-serif"],
                "body": ["Inter", "sans-serif"],
                "label": ["JetBrains Mono", "monospace"]
            },
            borderRadius: {
                // Design system uses sharp corners throughout; only pill/badge shapes use full rounding.
                "DEFAULT": "0px",
                "lg": "0px",
                "xl": "0px",
                "full": "9999px"
            },
        },
    },
    plugins: [
        require("@tailwindcss/forms"),
        require("@tailwindcss/container-queries"),
    ],
}
