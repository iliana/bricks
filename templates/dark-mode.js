(() => {
  const colorScheme = (() => {
    try {
      return window.localStorage.getItem("color-scheme");
    } catch (e) {
      return undefined;
    }
  })();

  const root = document.documentElement;

  if (
    colorScheme === "dark" ||
    (colorScheme !== "light" && window.matchMedia("(prefers-color-scheme: dark)").matches)
  ) {
    root.classList.add("dark");
  }

  document.querySelector("button#dark-mode-toggle").addEventListener("click", (event) => {
    try {
      window.localStorage.setItem("color-scheme", root.classList.toggle("dark") ? "dark" : "light");
    } catch (e) {}
  });
})();
