import cf from "cloudfront";
const kvs = cf.kvs();
async function handler(event) {
  const settings = await kvs.get("settings", { format: "string" });
  const binary = await kvs.get("binary", { format: "bytes" });
  const missing = await kvs.exists("missing");
  const meta = await kvs.meta();
  event.request.headers["x-settings"] = { value: settings };
  event.request.headers["x-binary"] = { value: binary.toString("base64") };
  event.request.headers["x-missing"] = { value: String(missing) };
  event.request.headers["x-count"] = { value: String(meta.keyCount) };
  return event.request;
}
