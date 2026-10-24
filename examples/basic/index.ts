import prompt from "./prompt.hbs";

const text: string = prompt({
  user: { name: "Ada", admin: true },
  items: [
    { title: "Write mapper", author: { name: "Ada" }, done: true },
    { title: "Ship it", author: { name: "Grace" } },
  ],
  tags: ["a", "b"],
});

console.log(text);
