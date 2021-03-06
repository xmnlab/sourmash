"""classify a stream of sequences"""

from sourmash.cli.utils import add_ksize_arg, add_moltype_args


def subparser(subparsers):
    subparser = subparsers.add_parser('watch')
    subparser.add_argument('sbt_name', help='name of SBT to search')
    subparser.add_argument('inp_file', nargs='?', default='/dev/stdin')
    subparser.add_argument(
        '-q', '--quiet', action='store_true',
        help='suppress non-error output'
    )
    subparser.add_argument(
        '-o', '--output',
        help='save signature generated from data here'
    )
    subparser.add_argument(
        '--threshold', metavar='T', default=0.05, type=float,
        help='minimum threshold for matches (default=0.05)'
    )
    subparser.add_argument(
        '--input-is-protein', action='store_true',
        help='Consume protein sequences - no translation needed'
    )
    add_moltype_args(subparser)
    subparser.add_argument(
        '-n', '--num-hashes', type=int, default=500,
        help='number of hashes to use in each sketch (default: %(default)i)'
    )
    subparser.add_argument(
        '--name', type=str, default='stdin',
        help='name to use for generated signature'
    )
    add_ksize_arg(subparser, 31)


def main(args):
    import sourmash
    return sourmash.commands.watch(args)
