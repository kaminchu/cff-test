import cf from "cloudfront";
const kvs = cf.kvs();
async function handler(event) {
  event.request.headers["x-setting"] = { value: await kvs.get("setting") };
  return event.request;
}
