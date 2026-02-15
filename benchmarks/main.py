import sqlite3
from fastapi import FastAPI
from fastapi.responses import JSONResponse

app = FastAPI()

DB_PATH = "./data.db"

@app.get("/people/{people_id}")
def read_person(people_id: int):
    # Connect to the database
    # check_same_thread=False is generally needed if you were sharing connections,
    # but strictly speaking, creating a new connection per request in a threadpool 
    # (which FastAPI does for sync functions) is thread-safe.
    with sqlite3.connect(DB_PATH) as conn:
        # row_factory allows us to convert rows to dicts easily (like -json)
        conn.row_factory = sqlite3.Row
        cursor = conn.cursor()
        
        # Execute query safely
        cursor.execute("SELECT * FROM people WHERE id = ?", (people_id,))
        row = cursor.fetchone()
        
        # Handle Not Found
        if row is None:
            return JSONResponse(
                status_code=404,
                content={"error": f"Person with id {people_id} not found"}
            )
        
        # Return the row as a dictionary
        return dict(row)