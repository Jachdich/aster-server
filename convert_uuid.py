import sqlite3
import shutil

shutil.copy("aster.db", "aster_backup_uuidconversion.db")

con = sqlite3.connect("aster.db")
cur = con.cursor()


for table in ["messages", "channels", "users", "emojis", "sync_servers"]:
    for row in list(cur.execute(f"SELECT uuid FROM {table}")):
        uuid, = row
        print(repr(uuid), 2**53-1, uuid > 2**53-1)
        if uuid > 2**53-1:
            new_uuid = uuid >> (63 - 53)
            print(f"Converting {uuid=} to {new_uuid=}")
            cur.execute(f"update {table} set uuid={new_uuid} where uuid={uuid}")
con.commit()
for row in list(cur.execute("SELECT uuid,author_uuid,channel_uuid FROM messages")):
    uuid, author_uuid, channel_uuid = row
    if author_uuid > 2**53-1:
        author_uuid = author_uuid >> (63 - 53)
    if channel_uuid > 2**53-1:
        channel_uuid = channel_uuid >> (63 - 53)

    cur.execute(f"update messages set author_uuid={author_uuid}, channel_uuid={channel_uuid} where uuid={uuid}")

con.commit()
for row in list(cur.execute("SELECT user_uuid FROM sync_data")):
    uuid, = row
    if uuid > 2**53-1:
        new_uuid = uuid >> (63 - 53)
        cur.execute(f"update sync_data set user_uuid={new_uuid} where user_uuid={uuid}")

con.commit()
for row in list(cur.execute("SELECT user_uuid FROM sync_servers")):
    uuid, = row
    if uuid > 2**53-1:
        new_uuid = uuid >> (63 - 53)
        cur.execute(f"update sync_servers set user_uuid={new_uuid} where user_uuid={uuid}")

con.commit()
