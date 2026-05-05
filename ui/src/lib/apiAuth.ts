const API_TOKEN_STORAGE_KEY = "helix.api.token.v1";

export function loadApiToken(): string {
  if (typeof window === "undefined") return "";
  try {
    return window.localStorage.getItem(API_TOKEN_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

export function saveApiToken(token: string): void {
  if (typeof window === "undefined") return;
  const normalized = token.trim();
  try {
    if (normalized) {
      window.localStorage.setItem(API_TOKEN_STORAGE_KEY, normalized);
    } else {
      window.localStorage.removeItem(API_TOKEN_STORAGE_KEY);
    }
  } catch {
    // Browser storage failures should not block API requests.
  }
}

export function clearApiToken(): void {
  saveApiToken("");
}
