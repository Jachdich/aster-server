import asyncio, ssl
from websockets import connect

async def hello(uri):
    ctx = ssl.SSLContext()
    async with connect(uri, ssl=ctx) as websocket:
        await websocket.send("Hello world!")
        await websocket.recv()

asyncio.run(hello("wss://localhost:2345"))
