function handler(event) {
  event.response.statusCode = 302;
  event.response.statusDescription = "Found";
  event.response.headers["location"] = { value: "/new" };
  return event.response;
}
