import cf from "cloudfront";
function handler(event) {
  return { value: cf.cwt };
}
