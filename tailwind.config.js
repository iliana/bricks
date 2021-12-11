module.exports = {
  content: ["./templates/*.{html,js,svg}", "./src/**/*.rs", "./node_modules/tablesort/dist/**.js"],
  darkMode: "class",
  plugins: [
    require("@tailwindcss/forms")({
      strategy: "class",
    }),
  ],
};
