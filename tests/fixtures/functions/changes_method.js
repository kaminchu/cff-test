function handler(event) {
  event.request.method = "POST";
  return event.request;
}
