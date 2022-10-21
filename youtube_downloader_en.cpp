#include <iostream>
#include <windows.h>
#include <string>
using namespace std;
HANDLE h = GetStdHandle(STD_OUTPUT_HANDLE);

bool isPlaylist();

//Music
void promptMusicPlaylist();
void createMusicPlaylistLink();
void createMusicNoPlaylistLink();
void createMusicNormalLink();

//Video
void promptVideoPlaylist();
void createVideoPlaylistLink();
void createVideoNoPlaylistLink();
void createVideoNormalLink();

struct{
    string link="";
    string command="";
} yt;

bool isPlaylist(){
    bool playlist;
    if(yt.link.find("list") != std::string::npos){
        playlist = true;
    }else playlist = false;
    return playlist;
}

//Music
void promptMusicPlaylist(){
    char r;
    cout<<endl<<"This video is in a playlist! Do you want to download the entire playlist? (y/n) ";
    cin>>r;
    if(r=='y'||r=='Y'){
        createMusicPlaylistLink();
    }else createMusicNoPlaylistLink();
}

void createMusicPlaylistLink(){
    yt.command = ".\\yt-dlp --ignore-errors --format bestaudio --extract-audio --audio-format mp3 --audio-quality 160K --output \"downloaded\\%(title)s.%(ext)s\" --yes-playlist \"" + yt.link + "\"";
}

void createMusicNoPlaylistLink(){
    yt.command = ".\\yt-dlp -x --no-playlist --audio-format mp3 --output \"downloaded\\%(title)s.%(ext)s\" \"" + yt.link + "\"";
}

void createMusicNormalLink(){
    yt.command = ".\\yt-dlp -x --audio-format mp3 --output \"downloaded\\%(title)s.%(ext)s\" \"" + yt.link + "\"";
}

//Video
void promptVideoPlaylist(){
    char r;
    cout<<endl<<"This video is in a playlist! Do you want to download the entire playlist? (y/n) ";
    cin>>r;
    if(r=='y'||r=='Y'){
        createVideoPlaylistLink();
    }else createVideoNoPlaylistLink();
}

void createVideoPlaylistLink(){
    yt.command = ".\\yt-dlp --yes-playlist -f mp4 --output \"downloaded\\%(title)s.%(ext)s\"  \"" + yt.link + "\"";
}

void createVideoNoPlaylistLink(){
    yt.command = ".\\yt-dlp -x --no-playlist -f mp4 --output \"downloaded\\%(title)s.%(ext)s\" \"" + yt.link + "\"";
}

void createVideoNormalLink(){
    yt.command = ".\\yt-dlp -f mp4 --output \"downloaded\\%(title)s.%(ext)s\" \"" + yt.link + "\"";
}

int main(){
    int scelta=0;
    char continua;
    bool cont;
    do{
        std::system("mkdir downloaded");
        std::system("cls");
        SetConsoleTextAttribute (h, 2);
        std::cout<<"[YouTube Downloader v1.1  ~  by Gab8bit]\n";
        SetConsoleTextAttribute (h, 7);
        std::cout<<"\n1- Audio\n2- Video\n\n> ";
        std::cin>>scelta;
        std::cout<<"\n\n";
        switch(scelta){
            case 1:{
                std::system("cls");
                SetConsoleTextAttribute (h, 9);
                std::cout<<"[YouTube Audio Downloader]";
                SetConsoleTextAttribute (h, 7);
                std::cout<<"\n\nVideo Link: ";
                std::cin>>yt.link;
                if(isPlaylist()){
                    promptMusicPlaylist();
                }else createMusicNormalLink();
                SetConsoleTextAttribute (h, 14);
                std::cout<<"\n\nDownload Status:\n";
                SetConsoleTextAttribute (h, 7);
                std::system(yt.command.c_str());
                std::cout<<"\n\n";
                break;
            };
            case 2:{
                std::system("cls");
                SetConsoleTextAttribute (h, 9);
                std::cout<<"[YouTube Video Downloader]";
                SetConsoleTextAttribute (h, 7);
                std::cout<<"\n\nVideo Link: ";
                std::cin>>yt.link;
                if(isPlaylist()){
                    promptVideoPlaylist();
                }else createVideoNormalLink();
                SetConsoleTextAttribute (h, 14);
                std::cout<<"\n\nDownload Status:\n";
                SetConsoleTextAttribute (h, 7);
                std::system(yt.command.c_str());
                std::cout<<"\n\n";
                break;
            };
        };
        SetConsoleTextAttribute (h, 2);
        cout<<endl<<"Finished!"<<endl<<"Do you want to download more audios or videos? (y/n) ";
        SetConsoleTextAttribute (h, 7);
        cin>>continua;
        if(continua=='y'||continua=='Y'){
            cont = true;
        }else cont = false;
    }while(cont);
}