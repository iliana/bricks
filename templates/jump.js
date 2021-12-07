(() => {
  const form = document.currentScript.parentElement;
  window.addEventListener("pageshow", () => {
    form.elements["path"].value = window.location.pathname;
  });
  form.querySelector("select").addEventListener("change", () => {
    window.location.assign(form.elements["path"].value);
  });
})();
