import orjson
from typing import Any, Dict, Union
from datetime import datetime
from ..utils.json_handler import json_loads, json_dumps


class APIResponseHandler:
    """Handle API responses with orjson."""
    
    def __init__(self):
        self.default_options = orjson.OPT_UTC_Z
    
    def parse_response(self, response_data: Union[str, bytes]) -> Dict[str, Any]:
        """Parse API response data."""
        try:
            if isinstance(response_data, str):
                response_data = response_data.encode('utf-8')
            return orjson.loads(response_data)
        except orjson.JSONDecodeError as e:
            raise ValueError(f"Invalid JSON response: {e}")
    
    def serialize_request(self, data: Dict[str, Any]) -> bytes:
        """Serialize request data for API calls."""
        try:
            return orjson.dumps(data, option=self.default_options)
        except TypeError as e:
            raise ValueError(f"Cannot serialize request data: {e}")
    
    def format_response(self, data: Dict[str, Any], *, pretty: bool = False) -> str:
        """Format response data for display."""
        options = self.default_options
        if pretty:
            options |= orjson.OPT_INDENT_2
        
        return orjson.dumps(data, option=options).decode('utf-8')
    
    def create_error_response(self, error_message: str, error_code: int = 400) -> str:
        """Create standardized error response."""
        error_data = {
            "error": {
                "message": error_message,
                "code": error_code,
                "timestamp": datetime.utcnow().isoformat() + "Z"
            }
        }
        return orjson.dumps(error_data, option=orjson.OPT_INDENT_2).decode('utf-8')