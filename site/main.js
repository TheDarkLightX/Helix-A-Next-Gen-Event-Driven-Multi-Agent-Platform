function setCopyButtonState(button, state) {
  if (!(button instanceof HTMLButtonElement)) return;
  if (state === "copied") {
    const prev = button.textContent;
    button.textContent = "Copied";
    button.dataset.prevLabel = prev ?? "Copy";
    button.disabled = true;
    setTimeout(() => {
      button.textContent = button.dataset.prevLabel ?? "Copy";
      button.disabled = false;
    }, 900);
  }
}

async function copyText(text) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const ta = document.createElement("textarea");
  ta.value = text;
  ta.setAttribute("readonly", "true");
  ta.style.position = "fixed";
  ta.style.left = "-9999px";
  document.body.appendChild(ta);
  ta.select();
  document.execCommand("copy");
  ta.remove();
}

function wireCopyButtons() {
  for (const button of document.querySelectorAll("[data-copy]")) {
    button.addEventListener("click", async () => {
      const text = button.getAttribute("data-copy");
      if (!text) return;
      try {
        await copyText(text);
        setCopyButtonState(button, "copied");
      } catch (err) {
        console.error("copy failed", err);
      }
    });
  }
}

function wireReveals() {
  const items = Array.from(document.querySelectorAll(".reveal"));
  if (items.length === 0) return;

  if (window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches) {
    for (const el of items) el.classList.add("is-visible");
    return;
  }

  const io = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting) e.target.classList.add("is-visible");
      }
    },
    { threshold: 0.12 }
  );

  for (const el of items) io.observe(el);
}

document.addEventListener("DOMContentLoaded", () => {
  wireCopyButtons();
  wireReveals();
});

