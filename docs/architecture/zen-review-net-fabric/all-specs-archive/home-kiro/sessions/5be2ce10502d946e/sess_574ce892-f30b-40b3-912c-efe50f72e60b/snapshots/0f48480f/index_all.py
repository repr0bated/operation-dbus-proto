#!/usr/bin/env python3
"""
Bulk indexer driver - walks repos, extracts symbols via tree-sitter,
reads source, and feeds into lspvec pipeline.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

