(() => {
  "use strict";

  const root = document.getElementById("integration-token-manager");
  if (!root) return;

  const listUrl = root.dataset.listUrl;
  const detailTemplate = root.dataset.detailTemplate;
  const form = document.getElementById("integration-token-form");
  const scopesContainer = document.getElementById("integration-scope-options");
  const tokenList = document.getElementById("integration-token-list");
  const emptyState = document.getElementById("integration-token-empty");
  const loadingState = document.getElementById("integration-token-loading");
  const status = document.getElementById("integration-token-status");
  const error = document.getElementById("integration-token-error");
  const secretPanel = document.getElementById("integration-token-secret-panel");
  const secretInput = document.getElementById("integration-token-secret");
  const secretTitle = document.getElementById("integration-token-secret-title");
  const copyButton = document.getElementById("integration-token-copy");
  const recommendedButton = document.getElementById("integration-token-nuvio-preset");
  const submitButton = document.getElementById("integration-token-submit");
  const csrfToken = form.querySelector("input[name='csrfmiddlewaretoken']").value;

  let recommendedScopes = [];

  const setStatus = (message) => {
    status.textContent = message;
  };

  const setError = (message) => {
    error.textContent = message;
    error.hidden = !message;
  };

  const apiMessage = (payload, fallback) =>
    payload?.error?.message || payload?.detail || fallback;

  const formatDate = (value) => {
    if (!value) return "Never";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "Unknown";
    return date.toLocaleString();
  };

  const detailUrl = (id) => detailTemplate.replace("/0/", `/${id}/`);

  const createText = (tag, text, className = "") => {
    const node = document.createElement(tag);
    node.textContent = text;
    if (className) node.className = className;
    return node;
  };

  const renderScopes = (availableScopes) => {
    scopesContainer.replaceChildren();
    availableScopes.forEach(({ scope, description }) => {
      const label = document.createElement("label");
      label.className =
        "flex gap-3 rounded-md border border-[var(--color-border)] p-3 cursor-pointer " +
        "hover:bg-[var(--color-surface-muted)] focus-within:ring-2 focus-within:ring-indigo-500";

      const checkbox = document.createElement("input");
      checkbox.type = "checkbox";
      checkbox.name = "scopes";
      checkbox.value = scope;
      checkbox.className = "mt-1 h-4 w-4 shrink-0";

      const text = document.createElement("span");
      text.className = "min-w-0";
      text.appendChild(
        createText("span", scope, "block font-mono text-sm text-[var(--color-text)]")
      );
      text.appendChild(
        createText(
          "span",
          description,
          "block text-xs text-[var(--color-text-muted)] mt-1"
        )
      );

      label.append(checkbox, text);
      scopesContainer.appendChild(label);
    });
  };

  const renderToken = (token) => {
    const card = document.createElement("li");
    card.className =
      "rounded-md border border-[var(--color-border)] bg-[var(--color-surface-dim)] p-4";

    const header = document.createElement("div");
    header.className = "flex flex-wrap items-start justify-between gap-3";

    const identity = document.createElement("div");
    identity.className = "min-w-0";
    identity.appendChild(
      createText("h3", token.name, "font-medium text-[var(--color-text)] break-words")
    );
    const stateText = `${token.state}. Prefix ${token.token_prefix || "not available"}.`;
    identity.appendChild(
      createText("p", stateText, "mt-1 text-sm text-[var(--color-text-muted)]")
    );

    const revoke = document.createElement("button");
    revoke.type = "button";
    revoke.className =
      "rounded-md border border-[var(--color-border)] px-3 py-2 text-sm " +
      "text-[var(--color-text-secondary)] hover:text-[var(--color-text)] " +
      "focus:outline-none focus:ring-2 focus:ring-indigo-500 disabled:opacity-60";
    revoke.textContent = token.state === "revoked" ? "Revoked" : "Revoke";
    revoke.disabled = token.state === "revoked";
    revoke.addEventListener("click", async () => {
      const confirmed = window.confirm(
        `Revoke “${token.name}”? Existing requests using this token will stop working.`
      );
      if (!confirmed) return;
      revoke.disabled = true;
      setError("");
      setStatus(`Revoking ${token.name}…`);
      try {
        const response = await fetch(detailUrl(token.id), {
          method: "DELETE",
          credentials: "same-origin",
          headers: { "X-CSRFToken": csrfToken },
        });
        if (!response.ok) {
          const payload = await response.json().catch(() => ({}));
          throw new Error(apiMessage(payload, "Could not revoke this token."));
        }
        setStatus(`${token.name} was revoked.`);
        await loadTokens();
      } catch (err) {
        revoke.disabled = false;
        setError(err.message || "Could not revoke this token.");
        setStatus("Token revocation needs attention.");
      }
    });

    header.append(identity, revoke);
    card.appendChild(header);

    const metadata = document.createElement("dl");
    metadata.className = "mt-3 grid gap-2 text-sm sm:grid-cols-2";
    const rows = [
      ["Created", formatDate(token.created_at)],
      ["Last used", formatDate(token.last_used_at)],
      ["Expires", formatDate(token.expires_at)],
      ["Client label", token.client_identifier || "Not set"],
    ];
    rows.forEach(([labelText, value]) => {
      const row = document.createElement("div");
      row.appendChild(
        createText("dt", labelText, "text-xs text-[var(--color-text-muted)]")
      );
      row.appendChild(
        createText("dd", value, "text-[var(--color-text-secondary)] break-words")
      );
      metadata.appendChild(row);
    });
    card.appendChild(metadata);

    const scopes = document.createElement("p");
    scopes.className = "mt-3 text-xs text-[var(--color-text-muted)] break-words";
    scopes.textContent = `Scopes: ${(token.scopes || []).join(", ") || "none"}`;
    card.appendChild(scopes);

    return card;
  };

  const loadTokens = async () => {
    loadingState.hidden = false;
    setError("");
    try {
      const response = await fetch(listUrl, { credentials: "same-origin" });
      const payload = await response.json().catch(() => ({}));
      if (!response.ok) {
        throw new Error(apiMessage(payload, "Could not load integration access."));
      }
      recommendedScopes = payload.recommended_nuvio_scopes || [];
      renderScopes(payload.available_scopes || []);
      tokenList.replaceChildren(...(payload.results || []).map(renderToken));
      emptyState.hidden = (payload.results || []).length !== 0;
      setStatus(
        (payload.results || []).length
          ? `${payload.results.length} scoped token${payload.results.length === 1 ? "" : "s"} loaded.`
          : "No scoped tokens are configured."
      );
    } catch (err) {
      tokenList.replaceChildren();
      emptyState.hidden = true;
      setError(
        `${err.message || "Could not load integration access."} Existing Floppy media data is unchanged.`
      );
      setStatus("Integration access needs attention.");
    } finally {
      loadingState.hidden = true;
    }
  };

  recommendedButton.addEventListener("click", () => {
    scopesContainer.querySelectorAll("input[name='scopes']").forEach((checkbox) => {
      checkbox.checked = recommendedScopes.includes(checkbox.value);
    });
    setStatus("Recommended Nuvio permissions selected. Review them before creating the token.");
  });

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    setError("");
    secretPanel.hidden = true;

    const formData = new FormData(form);
    const scopes = formData.getAll("scopes");
    if (!scopes.length) {
      setError("Select at least one permission.");
      setStatus("Token creation needs attention.");
      return;
    }

    const expiry = formData.get("expires_in_days");
    const body = {
      name: String(formData.get("name") || "").trim(),
      client_identifier: String(formData.get("client_identifier") || "").trim(),
      scopes,
      expires_in_days: expiry ? Number(expiry) : null,
    };

    submitButton.disabled = true;
    setStatus("Creating scoped token…");
    try {
      const response = await fetch(listUrl, {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          "X-CSRFToken": csrfToken,
        },
        body: JSON.stringify(body),
      });
      const payload = await response.json().catch(() => ({}));
      if (!response.ok) {
        throw new Error(apiMessage(payload, "Could not create this token."));
      }

      secretInput.value = payload.token;
      secretTitle.textContent = `${payload.name} is ready`;
      secretPanel.hidden = false;
      secretPanel.focus();
      setStatus("Token created. Copy the secret now; Floppy will not show it again.");
      form.reset();
      await loadTokens();
    } catch (err) {
      setError(err.message || "Could not create this token.");
      setStatus("Token creation needs attention.");
    } finally {
      submitButton.disabled = false;
    }
  });

  copyButton.addEventListener("click", async () => {
    if (!secretInput.value) return;
    try {
      await navigator.clipboard.writeText(secretInput.value);
      setStatus("Token secret copied.");
    } catch (_err) {
      secretInput.focus();
      secretInput.select();
      const copied = document.execCommand("copy");
      setStatus(copied ? "Token secret copied." : "Select the token secret and copy it manually.");
    }
  });

  loadTokens();
})();
