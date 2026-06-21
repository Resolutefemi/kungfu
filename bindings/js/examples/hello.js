// Hello-world example using the JS/TS binding.
//
//   npm install
//   npm run build
//   node examples/hello.js

const { Kungfu } = require('..');

const app = new Kungfu();

app.get('/hello', (req, res) => {
  res.json({ message: 'world', framework: 'kungfu', lang: 'javascript' });
});

app.post('/echo/:name', async (req, res) => {
  res.json({
    hello: req.params.name,
    youSent: req.body,
  });
});

app.listen(3000).then(() => console.log('🥋 kungfu listening on http://localhost:3000'));
