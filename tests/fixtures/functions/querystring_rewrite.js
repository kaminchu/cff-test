const querystring = require("querystring");
function handler(event) {
  event.request.headers["x-query"] = { value: JSON.stringify(querystring.parse("a=1&a=2&name=hello+world")) };
  return event.request;
}
