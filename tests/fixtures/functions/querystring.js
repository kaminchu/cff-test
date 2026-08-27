const querystring = require("querystring");
function handler(event) {
  return querystring.parse("a=1&a=2&name=hello+world");
}
