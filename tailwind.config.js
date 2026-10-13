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
                "on-accent": "var(--on-accent)",
                "hover": "var(--hover)",
                "primary-hover": "var(--primary-hover)",
                "on-primary-hover": "var(--on-primary-hover)",
                "focus": "var(--focus)",
            },
            // Control and card borders are a theme's "strong line" (black in
            // Bold, light grey in Corporate), not its ink.
            borderColor: {
                "black": "var(--line-strong)",
            },
            fontFamily: {
                "headline": ["Space Grotesk", "sans-serif"],
                "body": ["Inter", "sans-serif"],
                // Labels follow the theme; code, paths and queries stay mono.
                "label": ["var(--font-label)"],
                "mono": ["var(--font-mono)"]
            },
            borderRadius: {
                // Corners follow the theme (square in Bold); only pill shapes use full rounding.
                "DEFAULT": "var(--radius)",
                "lg": "var(--radius)",
                "xl": "var(--radius)",
                "full": "9999px"
            },
        },
    },
    plugins: [
        require("@tailwindcss/forms"),
        require("@tailwindcss/container-queries"),
    ],
}
