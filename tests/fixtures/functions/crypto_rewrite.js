const crypto = require("crypto");
function handler(event) {
  event.request.headers["x-digest"] = { value: crypto.createHash("sha256").update("hello").digest("hex") };
  return event.request;
}
