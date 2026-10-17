import argparse
import os
import multiprocessing


def main():
    multiprocessing.freeze_support()
    parser = argparse.ArgumentParser(description='Betun local podcast backend')
    parser.add_argument('--data-dir', help='Writable project/cache directory outside executable')
    parser.add_argument('--port', type=int, default=8765)
    parser.add_argument('--open', action='store_true', help='Open the local review UI in your browser')
    args = parser.parse_args()
    if args.data_dir:
        os.environ['BETUN_DATA_DIR'] = args.data_dir
    import uvicorn
    from betun.api import create_app, data_root
    app = create_app()
    import sys
    if args.open or (getattr(sys, 'frozen', False) and not os.environ.get('BETUN_NO_BROWSER')):
        import threading, time, webbrowser
        def open_browser():
            time.sleep(1.5)
            token = os.environ.get('BETUN_TOKEN') or (data_root() / 'api-token').read_text().strip()
            webbrowser.open(f'http://127.0.0.1:{args.port}/#token={token}')
        threading.Thread(target=open_browser, daemon=True).start()
    uvicorn.run(app, host='127.0.0.1', port=args.port, workers=1, access_log=False)


if __name__ == '__main__':
    main()
