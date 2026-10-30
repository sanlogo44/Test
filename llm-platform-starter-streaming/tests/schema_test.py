#!/usr/bin/env python3
"""Portable SQLite checks for the initial migration; not a substitute for Rust tests."""
from pathlib import Path
import sqlite3
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
MIGRATION = ROOT / "migrations" / "sqlite" / "0001_foundation.sql"

class FoundationSchemaTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.db_path = Path(self.tmp.name) / "test.db"
        self.db = sqlite3.connect(self.db_path)
        self.db.execute("PRAGMA foreign_keys = ON")
        self.db.executescript(MIGRATION.read_text(encoding="utf-8"))

    def tearDown(self):
        self.db.close()
        self.tmp.cleanup()

    def test_roles_are_seeded(self):
        names = {r[0] for r in self.db.execute("SELECT name FROM roles")}
        self.assertEqual(names, {"administrator", "operator", "user", "read_only"})

    def test_provider_and_model_registration_roundtrip(self):
        self.db.execute("INSERT INTO model_providers(id, name, provider_type) VALUES (?, ?, ?)", ("p1", "local", "local"))
        self.db.execute("INSERT INTO models(id, provider_id, external_model_id, display_name, source, local_path) VALUES (?, ?, ?, ?, ?, ?)", ("m1", "p1", "model-q4", "Model Q4", "local", "/models/model-q4.gguf"))
        row = self.db.execute("SELECT m.display_name, p.name, m.status FROM models m JOIN model_providers p ON p.id=m.provider_id").fetchone()
        self.assertEqual(row, ("Model Q4", "local", "registered"))

    def test_model_requires_existing_provider(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO models(id, provider_id, external_model_id, display_name) VALUES (?, ?, ?, ?)", ("m1", "missing", "x", "X"))

    def test_model_identifier_unique_per_provider(self):
        self.db.execute("INSERT INTO model_providers(id, name, provider_type) VALUES ('p1', 'local', 'local')")
        self.db.execute("INSERT INTO models(id, provider_id, external_model_id, display_name) VALUES ('m1', 'p1', 'x', 'X')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO models(id, provider_id, external_model_id, display_name) VALUES ('m2', 'p1', 'x', 'Duplicate X')")

    def test_json_configuration_must_be_valid(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO model_providers(id, name, provider_type, configuration_json) VALUES ('p1', 'bad-json', 'local', '{not-json')")

    def test_message_sequence_unique_per_conversation(self):
        self.db.execute("INSERT INTO conversations(id, title) VALUES ('c1', 'test')")
        self.db.execute("INSERT INTO messages(id, conversation_id, role, content, sequence_number) VALUES ('m1', 'c1', 'user', 'hello', 0)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO messages(id, conversation_id, role, content, sequence_number) VALUES ('m2', 'c1', 'assistant', 'world', 0)")

    def test_job_constraints_and_foreign_key(self):
        self.db.execute("INSERT INTO jobs(id, job_type, payload_json) VALUES ('j1', 'test', '{\"ok\":true}')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO jobs(id, job_type, maximum_attempts) VALUES ('j2', 'test', 0)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO job_attempts(id, job_id, attempt_number, worker_id, status) VALUES ('a1', 'missing-job', 1, 'w1', 'running')")

    def test_audit_metadata_must_be_valid_json(self):
        self.db.execute("INSERT INTO audit_events(id, action, resource_type, outcome, metadata_json) VALUES ('e1', 'model.registered', 'model', 'success', '{\"name\":\"tiny\"}')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO audit_events(id, action, resource_type, outcome, metadata_json) VALUES ('e2', 'model.registered', 'model', 'success', 'secret=abc')")

if __name__ == "__main__":
    unittest.main(verbosity=2)
