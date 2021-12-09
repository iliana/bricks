module.exports = {
  content: ["./templates/*.{html,js}", "./src/**/*.rs", "./node_modules/tablesort/dist/**.js"],
  darkMode: "class", // or 'media' or 'class'
  theme: {
    extend: {},
  },
  plugins: [
    require("@tailwindcss/forms")({
      strategy: "class",
    }),
  ],
};
