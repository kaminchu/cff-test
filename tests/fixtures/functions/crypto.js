const crypto = require("crypto");
function handler(event) {
  return { value: crypto.createHash("sha256").update("hello").digest("hex") };
}
