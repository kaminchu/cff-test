function handler(event) {
  event.request.uri = "/rewritten";
  return event.request;
}
