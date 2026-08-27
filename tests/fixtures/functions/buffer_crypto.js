const crypto = require("crypto");
function handler(event) {
  const digest = crypto.createHash("sha256").update(Buffer.from("hello")).digest();
  const hmac = crypto.createHmac("sha256", "key").update("The quick brown fox jumps over the lazy dog").digest("hex");
  event.request.headers["x-hash"] = { value: digest.toString("hex") };
  event.request.headers["x-bytes"] = { value: Buffer.from([1, 2, 3]).toString("base64url") };
  event.request.headers["x-hmac"] = { value: hmac };
  return event.request;
}
