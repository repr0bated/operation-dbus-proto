import os
import re
from pathlib import Path

def sanitize_filename(name):
    return re.sub(r'[\\/*?:"<>|]', "", name).strip()

def organize_files():
    directory = "research-notebook"
    files = list(Path(directory).glob("*"))
    
    print(f"Organizing {len(files)} files in {directory}...")
    
    for file_path in files:
        if not file_path.is_file():
            continue
            
        with open(file_path, "r", encoding="utf-8") as f:
            content = f.read()
            
        # Try to find an H1 title
        title_match = re.search(r"^#\s+(.+)$", content, re.MULTILINE)
        if title_match:
            title = title_match.group(1).strip()
        else:
            # Fallback: Use the first non-empty line
            lines = [line.strip() for line in content.split("\n") if line.strip()]
            if lines:
                title = lines[0]
            else:
                title = file_path.stem
                
        # Clean up title
        title = sanitize_filename(title)
        
        # New filename
        new_filename = f"{title}{file_path.suffix}"
        new_path = file_path.parent / new_filename
        
        if file_path.name != new_filename:
            print(f"Renaming: {file_path.name} -> {new_filename}")
            # Handle collision
            if new_path.exists():
                new_filename = f"{title}_{file_path.stem[:8]}{file_path.suffix}"
                new_path = file_path.parent / new_filename
            
            os.rename(file_path, new_path)
        else:
            print(f"Kept: {file_path.name}")

if __name__ == "__main__":
    organize_files()
