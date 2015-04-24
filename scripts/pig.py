#!/usr/bin/env python

import sys

# for pig
from pygments import highlight
from pygments.formatters import HtmlFormatter
from pygments.lexers import get_lexer_by_name
from pygments.util import ClassNotFound

import zmq
import threading

# for GDB
from pygments.lexer import RegexLexer, bygroups
from pygments.token import *

class TOMLLexer(RegexLexer):
    """
    Lexer for TOML, a simple language for config files
    """

    name = 'TOML'
    aliases = ['toml']
    filenames = ['*.toml']

    tokens = {
        'root': [

            # Basics, comments, strings
            (r'\s+', Text),
            (r'#.*?$', Comment.Single),
            (r'"(\\\\|\\"|[^"])*"', String),
            (r'(true|false)$', Keyword.Constant),
            ('[a-zA-Z_][a-zA-Z0-9_\-]*', Name),

            # Datetime
            (r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', Number.Integer),

            # Numbers
            (r'(\d+\.\d*|\d*\.\d+)([eE][+-]?[0-9]+)?j?', Number.Float),
            (r'\d+[eE][+-]?[0-9]+j?', Number.Float),
            (r'\-?\d+', Number.Integer),

            # Punctuation
            (r'[]{}:(),;[]', Punctuation),
            (r'\.', Punctuation),

            # Operators
            (r'=', Operator)

        ]
    }

# courtesy of https://github.com/snarez/gdb_lexer
class GDBLexer(RegexLexer):
    name = 'GDB'
    aliases = ['gdb']
    filenames = ['*.gdb']

    string = r'"[^"]*"'
    char = r'[a-zA-Z$._0-9@]'
    identifier = r'(?:[a-zA-Z$_]' + char + '*|\.' + char + '+)'
    number = r'(?:0[xX][a-zA-Z0-9]+|\d+)'

    tokens = {
        'root': [
            (r'\s+', Text),
            (r'(\(?gdb[\)\$]|>)( )('+identifier+')(/?)(\d*)(\w*)',
                bygroups(Keyword.Type, Text, Name.Builtin, Text, Literal.Number.Integer, Keyword.Constant)),
            (number, Number.Hex),
            (string, String),
            (r'=', Operator),
            (r'(\$\d+)( = {)', bygroups(Name.Variable, Text), 'struct'),
            (r'\$'+identifier+'+', Name.Variable),
            (r'\$'+number+'+', Name.Variable),
            (r'#.*', Comment),
            (r'<snip>', Comment.Special),
            (r'<'+identifier+'+\+?\d*>', Name.Function),
            (r'->'+identifier+'+', Name.Attribute),
            (r'(\()(\s*struct\s*'+identifier+'+\s*\*)(\))', bygroups(Text, Keyword.Type, Text)),
            (r'\((int|long|short|char)\s*\*?', Keyword.Type),
            (r'\b(if)\b', Name.Builtin),
            (r'.', Text),
        ],
        'struct': [
            (r'(\s*)([^\s]*)( = {)', bygroups(Text, Name.Variable, Text), '#push'),
            (r'(\s*)([^\s]*)( = )', bygroups(Text, Name.Variable, Text)),
            (r'\s*},?', Text, '#pop'),
            (number, Number.Hex),
            (string, String),
            (r'.', Text)
        ],
   }

class ServerTask(threading.Thread):
    """ServerTask"""
    def __init__(self, size):
        threading.Thread.__init__ (self)
        self.size = size

    def run(self):
        context = zmq.Context()
        frontend = context.socket(zmq.ROUTER)
        frontend.bind('tcp://127.0.0.1:5555')

        backend = context.socket(zmq.DEALER)
        backend.bind('inproc://backend')

        for i in range(self.size + 1):
            worker = ServerWorker(context)
            worker.daemon = True
            worker.start()

        zmq.proxy(frontend, backend)

        frontend.close()
        backend.close()
        context.term()

class ServerWorker(threading.Thread):
    def __init__(self, context):
        threading.Thread.__init__ (self)
        self.context = context
        self.formatter = HtmlFormatter(encoding='utf-8', nowrap=True)

    def run(self):
        socket = self.context.socket(zmq.REP)
        socket.connect('inproc://backend')

        while True:
            lang, code = socket.recv_multipart()

            lang = lang.decode(encoding="UTF-8")
            code = code.decode(encoding="UTF-8")

            rv = ""
            try:
                try:
                    if lang == "gdb":
                        lex = GDBLexer(encoding="utf-8")
                    elif lang == "toml":
                        lex = TOMLLexer(encoding="utf-8")
                    else:
                        lex = get_lexer_by_name(lang, encoding="utf-8")
                except ClassNotFound as err:
                    lex = get_lexer_by_name("text", encoding="utf-8")

                rv = highlight(code, lex, self.formatter)
            except ValueError as err:
                rv = "Pygments Error: {}".format(err)

            socket.send(rv)

        socket.close()

def main():
    server = ServerTask(4)
    server.daemon = True
    server.start()
    server.join()

if __name__ == "__main__":
    try:
        main()
    except (KeyboardInterrupt, SystemExit):
        sys.exit()
