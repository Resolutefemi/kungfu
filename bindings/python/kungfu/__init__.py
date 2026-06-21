"""Kungfu.js — Python binding.

Idiomatic Python API for the Kungfu.js polyglot web framework.

    from kungfu import Kungfu

    app = Kungfu()

    @app.get('/hello')
    def hello(req):
        return {'status': 200, 'headers': {}, 'body': {'message': 'world'}}

    @app.post('/echo/:name')
    async def echo(req):
        return {
            'status': 200,
            'headers': {},
            'body': {'hello': req['params']['name'], 'you_sent': req['body']},
        }

    app.run(port=3000)
"""

from ._native import Kungfu
from ._native import __version__

__all__ = ["Kungfu", "__version__"]
