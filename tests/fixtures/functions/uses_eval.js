function handler(event) {
  var evaluate = eval;
  evaluate("1 + 1");
  return event.request;
}
