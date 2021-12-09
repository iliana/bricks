module.exports = {
  purge: ["./templates/*.html", "./templates/*.js", "./src/**/*.rs", "./node_modules/tablesort/dist/**.js"],
  darkMode: "class", // or 'media' or 'class'
  theme: {
    extend: {},
  },
  variants: {
    extend: {},
  },
  plugins: [
    require("@tailwindcss/forms")({
      strategy: "class",
    }),
  ],
};
