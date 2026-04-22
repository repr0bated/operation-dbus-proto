from google.cloud import discoveryengine_v1alpha as discoveryengine
import os
from pathlib import Path

def upload_enterprise_sources(notebook_id):
    """
    Uses the official Google Cloud NotebookLM Enterprise API (v1alpha).
    Requires 'google-cloud-discoveryengine' and ADC authentication.
    """
    project_id = os.environ.get("GOOGLE_CLOUD_PROJECT", "dbus-enterprise-2026")
    location = "global"
    
    # In Enterprise, the notebook ID is usually a full resource name
    if not notebook_id.startswith("projects/"):
        notebook_name = f"projects/{project_id}/locations/{location}/notebooks/{notebook_id}"
    else:
        notebook_name = notebook_id

    client = discoveryengine.SourceServiceClient()
    directory = "research-notebook"
    
    print(f"Uploading organized sources to Enterprise Notebook: {notebook_name}")
    
    for file_path in Path(directory).glob("*"):
        if not file_path.is_file() or file_path.stat().st_size == 0:
            continue
            
        print(f"Uploading: {file_path.name}")
        try:
            # Note: The exact Enterprise API for source creation often involves 
            # GCS URIs or inline content. This is a template for the v1alpha flow.
            with open(file_path, "rb") as f:
                content = f.read()
                
            source = discoveryengine.Source(
                display_name=file_path.stem,
                inline_source=discoveryengine.InlineSource(
                    content=content,
                    mime_type="text/markdown" if file_path.suffix == ".md" else "text/plain"
                )
            )
            
            client.create_source(parent=notebook_name, source=source)
            print(f"  Success: {file_path.name}")
        except Exception as e:
            print(f"  Error: {e}")

if __name__ == "__main__":
    # Enterprise Notebook ID (if different from consumer)
    ENTERPRISE_NB_ID = "7808dae8-ed28-41d5-ae84-40e8be8a715a"
    upload_enterprise_sources(ENTERPRISE_NB_ID)
