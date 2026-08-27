function handler(event) {
  var first = Date.now();
  var second = new Date().getTime();
  var explicit = new Date(2020, 0).getDate();
  var called = Date();
  event.request.headers["x-date"] = { value: first + ":" + second + ":" + explicit + ":" + called };
  return event.request;
}
