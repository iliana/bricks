module.exports = {
  purge: ["./templates/*.html"],
  darkMode: "class", // or 'media' or 'class'
  theme: {
    screens: {
      sm: "640px",
      md: "768px",
      lg: "1024px",
      xl: "1280px",
    },
    extend: {},
  },
  variants: {
    extend: {},
  },
  plugins: [],
};
