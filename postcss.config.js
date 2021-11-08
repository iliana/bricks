module.exports = {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
    "@fullhuman/postcss-purgecss": {
      content: ["./templates/*.html"],
    },
    cssnano: {},
  },
};
