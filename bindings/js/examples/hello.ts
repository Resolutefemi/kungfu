// TypeScript hello-world example using the JS/TS binding.
//
//   npm install
//   npm run build
//   npx ts-node examples/hello.ts

import { Kungfu } from '..';

const app = new Kungfu();

app.get('/hello', (_req, res) => {
  res.json({ message: 'world', framework: 'kungfu', lang: 'typescript' });
});

app.post('/echo/:name', async (req, res) => {
  res.json({
    hello: req.params.name,
    youSent: req.body,
  });
});

app.listen(3000).then(() => console.log('🥋 kungfu listening on http://localhost:3000'));
