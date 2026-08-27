function handler(event) {
  const bytes = new TextEncoder().encode("あ");
  const text = new TextDecoder().decode(bytes);
  event.request.headers["x-text"] = { value: text + ":" + bytes.length };
  return event.request;
}
