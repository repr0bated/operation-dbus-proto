import asyncio
import os
from pathlib import Path
from notebooklm import NotebookLMClient

async def upload_organized_notes(notebook_id):
    directory = "research-notebook"
    files = list(Path(directory).glob("*"))
    
    print(f"Connecting to NotebookLM...")
    try:
        # This requires having logged in via 'notebooklm login' CLI previously
        async with await NotebookLMClient.from_storage() as client:
            print(f"Uploading {len(files)} sources to notebook {notebook_id}...")
            
            for file_path in files:
                if file_path.stat().st_size == 0:
                    print(f"Skipping empty file: {file_path.name}")
                    continue
                    
                print(f"Uploading: {file_path.name}")
                try:
                    await client.sources.upload(notebook_id, str(file_path))
                    print(f"  Done: {file_path.name}")
                except Exception as e:
                    print(f"  Error uploading {file_path.name}: {e}")
            
            print("\nOrganization complete! Sources are now titled correctly in NotebookLM.")
            
    except Exception as e:
        print(f"Authentication Error: {e}")
        print("\nPlease run 'notebooklm login' in your terminal first to authorize access.")

if __name__ == "__main__":
    NOTEBOOK_ID = "7808dae8-ed28-41d5-ae84-40e8be8a715a"
    asyncio.run(upload_organized_notes(NOTEBOOK_ID))
