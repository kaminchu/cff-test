function handler(event) {
  event.request.headers["x-now"] = { value: String(Date.now()) };
  return event.request;
}
