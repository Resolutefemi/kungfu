"""FastAPI hello-world server, for direct comparison with kungfu's bench.

Run with:
    uvicorn server:app --host 127.0.0.1 --port 3003
    oha -z 5s -c 64 http://localhost:3003/hello
"""
from fastapi import FastAPI

app = FastAPI()


@app.get("/hello")
async def hello():
    return {"message": "world"}
