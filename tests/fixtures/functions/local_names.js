function handler(event) {
  var process = { fetch: "ok" };
  event.request.headers["x-local"] = { value: process.fetch };
  return event.request;
}
