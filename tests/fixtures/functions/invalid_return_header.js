function handler(event) {
  event.request.headers.Host = { value: "example.com" };
  return event.request;
}
